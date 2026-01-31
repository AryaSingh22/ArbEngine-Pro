const { Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell, 
        HeadingLevel, AlignmentType, BorderStyle, WidthType, ShadingType,
        PageBreak, LevelFormat } = require('docx');
const fs = require('fs');

const doc = new Document({
  styles: {
    default: { 
      document: { 
        run: { font: "Arial", size: 24 } 
      } 
    },
    paragraphStyles: [
      { 
        id: "Heading1", 
        name: "Heading 1", 
        basedOn: "Normal", 
        next: "Normal", 
        quickFormat: true,
        run: { size: 32, bold: true, font: "Arial" },
        paragraph: { 
          spacing: { before: 480, after: 240 }, 
          outlineLevel: 0 
        } 
      },
      { 
        id: "Heading2", 
        name: "Heading 2", 
        basedOn: "Normal", 
        next: "Normal", 
        quickFormat: true,
        run: { size: 28, bold: true, font: "Arial" },
        paragraph: { 
          spacing: { before: 360, after: 180 }, 
          outlineLevel: 1 
        } 
      },
      { 
        id: "Heading3", 
        name: "Heading 3", 
        basedOn: "Normal", 
        next: "Normal", 
        quickFormat: true,
        run: { size: 26, bold: true, font: "Arial" },
        paragraph: { 
          spacing: { before: 240, after: 120 }, 
          outlineLevel: 2 
        } 
      },
    ]
  },
  numbering: {
    config: [
      { 
        reference: "bullets",
        levels: [
          { 
            level: 0, 
            format: LevelFormat.BULLET, 
            text: "•", 
            alignment: AlignmentType.LEFT,
            style: { 
              paragraph: { 
                indent: { left: 720, hanging: 360 } 
              } 
            } 
          }
        ] 
      },
      { 
        reference: "numbers",
        levels: [
          { 
            level: 0, 
            format: LevelFormat.DECIMAL, 
            text: "%1.", 
            alignment: AlignmentType.LEFT,
            style: { 
              paragraph: { 
                indent: { left: 720, hanging: 360 } 
              } 
            } 
          }
        ] 
      },
    ]
  },
  sections: [{
    properties: {
      page: {
        size: {
          width: 12240,
          height: 15840
        },
        margin: { 
          top: 1440, 
          right: 1440, 
          bottom: 1440, 
          left: 1440 
        }
      }
    },
    children: [
      // Title Page
      new Paragraph({
        alignment: AlignmentType.CENTER,
        spacing: { before: 2880, after: 480 },
        children: [
          new TextRun({
            text: "Solana Arbitrage Dashboard",
            size: 48,
            bold: true
          })
        ]
      }),
      new Paragraph({
        alignment: AlignmentType.CENTER,
        spacing: { after: 240 },
        children: [
          new TextRun({
            text: "Product Requirements Document",
            size: 32
          })
        ]
      }),
      new Paragraph({
        alignment: AlignmentType.CENTER,
        spacing: { after: 1440 },
        children: [
          new TextRun({
            text: "Version 1.0 - January 2026",
            size: 24,
            italics: true
          })
        ]
      }),
      
      // Document Info Table
      new Table({
        width: { size: 100, type: WidthType.PERCENTAGE },
        columnWidths: [3000, 6360],
        rows: [
          new TableRow({
            children: [
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                shading: { fill: "D5E8F0", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Project Name", bold: true })] })]
              }),
              new TableCell({
                width: { size: 6360, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Solana Arbitrage Dashboard & Trading System")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                shading: { fill: "D5E8F0", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Technology Stack", bold: true })] })]
              }),
              new TableCell({
                width: { size: 6360, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Rust, Solana SDK, React, PostgreSQL")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                shading: { fill: "D5E8F0", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Total Timeline", bold: true })] })]
              }),
              new TableCell({
                width: { size: 6360, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Phase 1: 9-10 weeks | Phase 2: 11-12 weeks")] })]
              })
            ]
          })
        ]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Table of Contents placeholder
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("Table of Contents")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [new TextRun("1. Executive Summary")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [new TextRun("2. Product Vision")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [new TextRun("3. Phase 1: Arbitrage Dashboard - Requirements")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [new TextRun("4. Phase 2: Triangular Arbitrage Bot - Requirements")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [new TextRun("5. Development Roadmap")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [new TextRun("6. Risk Analysis and Mitigation")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [new TextRun("7. Success Metrics")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [new TextRun("8. Appendix: Key Technologies")]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Executive Summary
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("1. Executive Summary")]
      }),
      new Paragraph({
        spacing: { after: 240 },
        children: [
          new TextRun("This document outlines the requirements and architecture for a Solana-based arbitrage opportunity identification and trading system. The project will be developed in two phases:")
        ]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Phase 1: Real-time Arbitrage Dashboard - A monitoring system that identifies and displays arbitrage opportunities across Solana DEXs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Phase 2: Automated Triangular Arbitrage Bot - An execution engine that automatically identifies optimal triangular arbitrage paths and executes profitable trades")]
      }),
      new Paragraph({
        spacing: { before: 240, after: 240 },
        children: [
          new TextRun("The system will leverage Rust for high-performance computation, Solana&#x2019;s low-latency blockchain infrastructure, and real-time data feeds from major DEXs including Raydium, Orca, and Jupiter.")
        ]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        children: [new TextRun("1.1 Key Objectives")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Detect arbitrage opportunities with sub-second latency")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Provide clear, actionable insights through an intuitive dashboard")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Execute triangular arbitrage trades automatically with minimal risk")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Maintain profitability after all fees and costs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Build a scalable, maintainable codebase in Rust")]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Product Vision
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("2. Product Vision")]
      }),
      
      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        children: [new TextRun("2.1 Problem Statement")]
      }),
      new Paragraph({
        spacing: { after: 240 },
        children: [
          new TextRun("Decentralized exchanges on Solana experience price inefficiencies due to fragmented liquidity, varying trading volumes, and network latency. These inefficiencies create arbitrage opportunities that typically last only seconds. Manual identification and execution of these opportunities is impractical due to:")
        ]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Speed requirements - opportunities disappear within 1-5 seconds")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Computational complexity - requires real-time monitoring of multiple trading pairs across multiple DEXs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Transaction costs - Solana gas fees and DEX swap fees must be factored into profitability calculations")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Execution risk - slippage and failed transactions can turn profitable opportunities into losses")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        spacing: { before: 360 },
        children: [new TextRun("2.2 Target Users")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Crypto traders seeking to understand and exploit arbitrage opportunities")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Quantitative researchers analyzing DEX market efficiency")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Market makers optimizing their trading strategies")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("DeFi developers building on Solana")]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Phase 1 Requirements
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("3. Phase 1: Arbitrage Dashboard - Requirements")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        children: [new TextRun("3.1 Core Features")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        children: [new TextRun("3.1.1 Real-Time Price Monitoring")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Monitor top 50 trading pairs by volume on Solana DEXs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Support for major DEXs: Raydium, Orca, Jupiter Aggregator")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Update frequency: Sub-second refresh (target: 100-500ms)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Price data includes: bid, ask, mid-price, 24h volume, liquidity depth")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        spacing: { before: 240 },
        children: [new TextRun("3.1.2 Arbitrage Opportunity Detection")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Identify simple arbitrage: same pair, different DEXs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Calculate net profit after fees (DEX fees + Solana transaction fees)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Configurable minimum profit threshold (default: 0.5%)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Display opportunity lifespan and frequency metrics")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        spacing: { before: 240 },
        children: [new TextRun("3.1.3 Dashboard Interface")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Live opportunities table: sorted by profitability, filterable by pair/DEX")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Price comparison view: side-by-side pricing across DEXs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Historical charts: opportunity frequency, average profit, market conditions")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Alert panel: notification system for opportunities above threshold")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        spacing: { before: 360 },
        children: [new TextRun("3.2 Technical Requirements")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        children: [new TextRun("3.2.1 Performance")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Price update latency: < 500ms from DEX to dashboard")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Dashboard refresh rate: 1-2 updates per second")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Support concurrent monitoring of 50+ pairs without degradation")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Memory footprint: < 512MB for core monitoring service")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        spacing: { before: 240 },
        children: [new TextRun("3.2.2 Data Storage")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Time-series database for historical price data (TimescaleDB)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Opportunity log: store all detected opportunities with metadata")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Retention policy: 90 days for detailed data, aggregated data indefinitely")]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Phase 2 Requirements
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("4. Phase 2: Triangular Arbitrage Bot - Requirements")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        children: [new TextRun("4.1 Core Features")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        children: [new TextRun("4.1.1 Triangular Path Discovery")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Identify all possible 3-hop trading paths (e.g., SOL → USDC → RAY → SOL)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Calculate expected profit for each path considering all fees")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Rank paths by expected profit and execution probability")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Support multi-DEX paths (trade on Raydium, Orca, Jupiter in sequence)")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        spacing: { before: 240 },
        children: [new TextRun("4.1.2 Smart Execution Engine")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Atomic transaction bundling - all three trades succeed or all revert")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Slippage protection with configurable tolerance (default: 1%)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Priority fee optimization for transaction inclusion")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Transaction simulation before submission to prevent failures")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        spacing: { before: 240 },
        children: [new TextRun("4.1.3 Risk Management")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Maximum position size per trade (configurable, default: $1,000)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Daily loss limit circuit breaker")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Minimum profit threshold after all costs (default: 0.3%)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Whitelist of approved tokens and DEXs")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        spacing: { before: 360 },
        children: [new TextRun("4.2 Technical Requirements")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        children: [new TextRun("4.2.1 Execution Speed")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("End-to-end latency (detection to submission): < 100ms")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Path calculation time: < 50ms for 100+ possible paths")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Transaction signing and submission: < 20ms")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_3,
        spacing: { before: 240 },
        children: [new TextRun("4.2.2 Security")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Secure key management (hardware wallet support)")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Transaction simulation to prevent rug pulls")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Rate limiting to prevent spam")]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Development Roadmap
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("5. Development Roadmap")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        children: [new TextRun("5.1 Phase 1 Timeline (9-10 weeks)")]
      }),
      
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Weeks 1-2: Data Collector for Raydium, Orca, Jupiter")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Weeks 3-4: Arbitrage Detector and Database Setup")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Week 5: REST API and WebSocket Server")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Weeks 6-7: React Dashboard with Live Updates")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Week 8: Charts, Analytics, and Alerts")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Week 9-10: Testing, Optimization, Documentation")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        spacing: { before: 360 },
        children: [new TextRun("5.2 Phase 2 Timeline (11-12 weeks)")]
      }),
      
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Weeks 1-2: Path Optimizer with Bellman-Ford Algorithm")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Week 3: Path Validation and Liquidity Checks")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Weeks 4-5: Solana Transaction Builder and Atomic Swaps")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Week 6: Transaction Simulation and Wallet Integration")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Week 7: Risk Management Module")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Week 8: P&L Dashboard and Performance Metrics")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Weeks 9-10: End-to-End Testing and Backtesting")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Weeks 11-12: Live Testing and Production Deployment")]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Risk Analysis
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("6. Risk Analysis and Mitigation")]
      }),

      new Table({
        width: { size: 100, type: WidthType.PERCENTAGE },
        columnWidths: [2800, 3800, 2760],
        rows: [
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2800, type: WidthType.DXA },
                shading: { fill: "2E5C8A", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Risk", bold: true, color: "FFFFFF" })] })]
              }),
              new TableCell({
                width: { size: 3800, type: WidthType.DXA },
                shading: { fill: "2E5C8A", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Impact", bold: true, color: "FFFFFF" })] })]
              }),
              new TableCell({
                width: { size: 2760, type: WidthType.DXA },
                shading: { fill: "2E5C8A", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Mitigation", bold: true, color: "FFFFFF" })] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2800, type: WidthType.DXA },
                shading: { fill: "FFE6E6", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "High Network Latency", bold: true })] })]
              }),
              new TableCell({
                width: { size: 3800, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Opportunities disappear before execution")] })]
              }),
              new TableCell({
                width: { size: 2760, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Use premium RPC providers, co-locate servers")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2800, type: WidthType.DXA },
                shading: { fill: "FFE6E6", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Slippage Risk", bold: true })] })]
              }),
              new TableCell({
                width: { size: 3800, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Execution price worse than expected")] })]
              }),
              new TableCell({
                width: { size: 2760, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Conservative slippage limits, liquidity validation")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2800, type: WidthType.DXA },
                shading: { fill: "FFF4E6", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Smart Contract Vulnerabilities", bold: true })] })]
              }),
              new TableCell({
                width: { size: 3800, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Malicious contracts could drain funds")] })]
              }),
              new TableCell({
                width: { size: 2760, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Whitelist trusted tokens, simulate transactions")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2800, type: WidthType.DXA },
                shading: { fill: "E6F4FF", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Competition from Bots", bold: true })] })]
              }),
              new TableCell({
                width: { size: 3800, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Other arbitrageurs close opportunities faster")] })]
              }),
              new TableCell({
                width: { size: 2760, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Optimize execution speed, predictive algorithms")] })]
              })
            ]
          })
        ]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Success Metrics
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("7. Success Metrics")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        children: [new TextRun("7.1 Phase 1 Success Criteria")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Detect 95%+ of actual arbitrage opportunities across monitored DEXs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("False positive rate < 5% for opportunity detection")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Dashboard latency < 500ms from DEX price update")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("System uptime > 99% during market hours")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("User can filter and sort opportunities in < 100ms")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        spacing: { before: 360 },
        children: [new TextRun("7.2 Phase 2 Success Criteria")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("70%+ of executed trades are profitable after all fees")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Average profit per trade > 0.5% after costs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("End-to-end execution latency < 100ms")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Transaction failure rate < 10%")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Positive total P&L after 30 days of operation")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Zero security incidents or fund losses")]
      }),

      new Paragraph({ children: [new PageBreak()] }),

      // Appendix
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun("8. Appendix: Key Technologies")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        children: [new TextRun("8.1 Why Rust?")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Performance: Near C/C++ speed, critical for sub-100ms latency requirements")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Memory Safety: Prevents common bugs that could cause fund loss")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Concurrency: Excellent async support for handling multiple DEX connections")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Ecosystem: Native Solana SDK support, mature crypto libraries")]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        spacing: { before: 360 },
        children: [new TextRun("8.2 Core Dependencies")]
      }),

      new Table({
        width: { size: 100, type: WidthType.PERCENTAGE },
        columnWidths: [2500, 3000, 3860],
        rows: [
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2500, type: WidthType.DXA },
                shading: { fill: "2E5C8A", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Category", bold: true, color: "FFFFFF" })] })]
              }),
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                shading: { fill: "2E5C8A", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Technology", bold: true, color: "FFFFFF" })] })]
              }),
              new TableCell({
                width: { size: 3860, type: WidthType.DXA },
                shading: { fill: "2E5C8A", type: ShadingType.CLEAR },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun({ text: "Purpose", bold: true, color: "FFFFFF" })] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2500, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Blockchain")] })]
              }),
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("solana-sdk, solana-client")] })]
              }),
              new TableCell({
                width: { size: 3860, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Transaction building, signing, submission")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2500, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Async Runtime")] })]
              }),
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("tokio")] })]
              }),
              new TableCell({
                width: { size: 3860, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Asynchronous execution, concurrency")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2500, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("WebSocket")] })]
              }),
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("tokio-tungstenite")] })]
              }),
              new TableCell({
                width: { size: 3860, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Real-time DEX connections")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2500, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Web Framework")] })]
              }),
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("axum or actix-web")] })]
              }),
              new TableCell({
                width: { size: 3860, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("REST API and WebSocket server")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2500, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Database")] })]
              }),
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("PostgreSQL + TimescaleDB")] })]
              }),
              new TableCell({
                width: { size: 3860, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Time-series price data storage")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2500, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Message Queue")] })]
              }),
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Redis Streams")] })]
              }),
              new TableCell({
                width: { size: 3860, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Inter-component communication")] })]
              })
            ]
          }),
          new TableRow({
            children: [
              new TableCell({
                width: { size: 2500, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Frontend")] })]
              }),
              new TableCell({
                width: { size: 3000, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("React + TypeScript")] })]
              }),
              new TableCell({
                width: { size: 3860, type: WidthType.DXA },
                margins: { top: 80, bottom: 80, left: 120, right: 120 },
                children: [new Paragraph({ children: [new TextRun("Dashboard interface")] })]
              })
            ]
          })
        ]
      }),

      new Paragraph({
        heading: HeadingLevel.HEADING_2,
        spacing: { before: 480 },
        children: [new TextRun("8.3 DEX Integration Details")]
      }),
      new Paragraph({
        spacing: { after: 120 },
        children: [
          new TextRun({
            text: "Raydium",
            bold: true
          })
        ]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("WebSocket API for real-time price feeds")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Program: RaydiumAMM")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Typical fee: 0.25%")]
      }),

      new Paragraph({
        spacing: { before: 240, after: 120 },
        children: [
          new TextRun({
            text: "Orca",
            bold: true
          })
        ]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Whirlpools for concentrated liquidity")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("REST API for current prices, WebSocket for updates")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Typical fee: 0.30%")]
      }),

      new Paragraph({
        spacing: { before: 240, after: 120 },
        children: [
          new TextRun({
            text: "Jupiter",
            bold: true
          })
        ]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Aggregator - routes through multiple DEXs")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Price API for quote comparison")]
      }),
      new Paragraph({
        numbering: { reference: "bullets", level: 0 },
        children: [new TextRun("Fee varies by route")]
      }),

    ]
  }]
});

Packer.toBuffer(doc).then(buffer => {
  fs.writeFileSync("/mnt/user-data/outputs/Solana_Arbitrage_PRD.docx", buffer);
  console.log("PRD document created successfully!");
});
